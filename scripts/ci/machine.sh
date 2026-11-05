#!/usr/bin/env bash
# For CI: writes the runner's facts into the job's summary and log. On Linux it also lets the
# tests read the CPU's counters (perf_event_paranoid). Usage: bash scripts/ci/machine.sh <target>
set -u

target="${1:-unknown}"
summary="${GITHUB_STEP_SUMMARY:-/dev/null}"

yes_no() { # yes_no <word> <the line to look in>
  if grep -qw "$1" <<< "$2"; then echo "- $1: yes"; else echo "- $1: no"; fi
}

{
  echo "### The machine ($target)"
  echo
  case "$target" in
    linux-x86_64)
      sudo sysctl kernel.perf_event_paranoid=1 > /dev/null || true
      echo "$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //'), $(nproc) vCPUs, kernel $(uname -r)"
      echo
      flags=$(grep -m1 '^flags' /proc/cpuinfo || true)
      for flag in sse4_2 popcnt pclmulqdq aes sha_ni avx2 bmi1 bmi2 avx512bw; do yes_no "$flag" "$flags"; done
      echo "- AVX-512: $(grep -o 'avx512[a-z]*' /proc/cpuinfo | sort -u | tr '\n' ' ')"
      echo "- LSMs: $(cat /sys/kernel/security/lsm 2> /dev/null || echo unknown)"
      echo "- ptrace_scope: $(cat /proc/sys/kernel/yama/ptrace_scope 2> /dev/null || echo none)"
      ;;
    linux-aarch64)
      sudo sysctl kernel.perf_event_paranoid=1 > /dev/null || true
      echo "ARM64, $(nproc) vCPUs, kernel $(uname -r)"
      echo
      features=$(grep -m1 '^Features' /proc/cpuinfo || true)
      for feature in asimd crc32 pmull aes sha1 sha2 atomics sve sve2; do yes_no "$feature" "$features"; done
      echo "- LSMs: $(cat /sys/kernel/security/lsm 2> /dev/null || echo unknown)"
      echo "- ptrace_scope: $(cat /proc/sys/kernel/yama/ptrace_scope 2> /dev/null || echo none)"
      ;;
    macos-*)
      echo "$(sysctl -n machdep.cpu.brand_string 2> /dev/null || uname -m), $(sysctl -n hw.ncpu) vCPUs, $(($(sysctl -n hw.memsize) / 1073741824)) GiB, macOS $(sw_vers -productVersion) ($(sw_vers -buildVersion))"
      echo
      echo '```'
      sysctl -a 2> /dev/null | grep -E '^hw\.optional\.(arm\.FEAT_(CRC32|PMULL|AES|SHA256|SHA3|SVE|SME)|avx|sse4|aes|bmi|popcnt)' | sort || true
      echo '```'
      ;;
    windows-*)
      echo "${PROCESSOR_IDENTIFIER:-unknown}, ${NUMBER_OF_PROCESSORS:-?} vCPUs, $(cmd //c ver 2> /dev/null | tr -d '\r' | sed '/^$/d')"
      ;;
    *)
      uname -a
      ;;
  esac
} | tee -a "$summary"

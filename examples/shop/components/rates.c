#include "tax.h"

uint64_t exports_lengua_tax_rates_tax_of(uint64_t cents) {
    return (cents * 8 + 50) / 100;
}

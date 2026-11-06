---
type: lengua/program-idea
title: nap
description: Sleeps for half a minute and then says that it is awake.
---
# Purpose
The program waits `30` seconds on the clock that only moves forward. Then it prints `awake`, for a
script that needs a pause. If the wait fails, it exits with code 2. If the program cannot write its
output, it exits with code 3.

# Behaviors
## B1: Says that it is awake after its nap
- Given no arguments
- When the program runs
- Then it prints `awake` and a newline
- And it exits with code `0`

# Constraints
- Uses: the clock

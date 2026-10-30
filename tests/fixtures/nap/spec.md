---
type: lengua/program-spec
title: nap
description: Naps for half a minute, then says it's awake.
---
# Purpose
Waits `30` seconds on the clock that only moves forward, then prints `awake`, for a script that
needs a pause. If the wait or the writing fails, it exits with code 2 or 3.

# Behaviors
## B1: Says it's awake after its nap
- Given no arguments
- When the program runs
- Then it prints `awake` followed by a newline
- And it exits with code `0`

# Constraints
- Uses: the clock

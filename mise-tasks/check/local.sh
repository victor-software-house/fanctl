#!/bin/sh
#MISE description="This repository's own checks"
#MISE depends=[]
# check:repo depends on this task, so it stays a file. A repository lists its
# own checks in depends, each a task that runs one tool; the dependencies run
# in parallel. A starter: the template writes this file once and never
# rewrites it.

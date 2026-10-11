The workspace holds the cabin of a vehicle and the system that deploys it. The
telemetry component on the gateway machine must now record the horn's health and
how many times the horn has sounded since start-up; the horn does not publish
that count yet. Change the workspace so that the telemetry component receives
both, without breaking any existing consumer of the cabin package. Show that the
change is compatible, and explain which part of the change a deployed consumer
can observe.

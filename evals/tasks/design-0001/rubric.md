1. **must** provide RIDL declarations for system/component identity and
   announcement metadata, with an interface interaction that represents repeated
   presence announcements.

2. **must** keep component identity distinct from component type and avoid
   deriving the latter from the numeric component identifier.

3. **must** represent or document a monitor result that distinguishes connected
   and disconnected components and relates each result to its system/component
   identity.

4. **must** make the announcement interval and missed-announcement limit
   explicit deployment choices rather than claim a universal protocol-mandated
   timeout.

5. **must** explain that continued announcements while operational, suppression
   while faulted and disconnect detection require runtime state and scheduling
   beyond a compiling RIDL schema.

6. **should** group discovery concerns in a focused interface rather than
   combine them with unrelated mission or parameter operations.

7. **must not** claim that the generated types or RIDL timing annotations alone
   implement discovery or disconnect detection.

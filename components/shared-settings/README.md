# Shared Settings

This is an EXPERIMENTAL proof-of-concept for a "settings" API that syncs.
It's different from desktop's "preferences" engine in that instead of syncing implementation
details about what exact preference names are used, it is intended to sync concepts which apply
to all platforms - hence "shared": the settings are shared between desktop, Android and iOS,
rather than being private to one app.

For example, you could imagine the (now dated) "world cup widget" might want
to sync the teams you follow.

# E2E_CASE selection spans real-server suites

After query and lifecycle coverage share one Make target, an `E2E_CASE` pair may be applicable to
only one suite: a non-owner shared-server pair has queries only, while Java and Lua have detached
lifecycle coverage but excluded direct-query smoke. Each suite must validate selection against all
declared pairs and then run only applicable cases; requiring a local match breaks valid selection.

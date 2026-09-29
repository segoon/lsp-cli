ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../..)
RUNNER := cargo test --locked --test e2e-runner --
SELECTOR_ARGS := $(if $(CASE),--case $(CASE),$(if $(SERVER),--server $(SERVER),))
PHASES := $(if $(filter-out all,$(PHASE)),$(PHASE),provision smoke lifecycle)

# Keep each case's diagnostics together when GNU Make runs several runner processes concurrently.
MAKEFLAGS += --output-sync=target

.PHONY: run phase phase-tasks

run:
	+@status=0; \
	for phase in $(PHASES); do \
		$(MAKE) --no-print-directory --keep-going -f tests/e2e/Makefile phase \
			SUITE="$(SUITE)" E2E_PHASE="$$phase" CASE="$(CASE)" SERVER="$(SERVER)" \
			E2E_SHARD_DIR="$(E2E_SHARD_DIR)" || status=1; \
	done; \
	exit "$$status"

phase:
	+@units="$$(cd "$(ROOT)" && $(RUNNER) --suite "$(SUITE)" --phase "$(E2E_PHASE)" \
		$(SELECTOR_ARGS) --list-work)" || exit; \
	$(MAKE) --no-print-directory --keep-going --output-sync=target \
		-f tests/e2e/Makefile phase-tasks \
		SUITE="$(SUITE)" E2E_PHASE="$(E2E_PHASE)" CASE="$(CASE)" SERVER="$(SERVER)" \
		E2E_SHARD_DIR="$(E2E_SHARD_DIR)" E2E_UNITS="$$units"

phase-tasks: $(addprefix e2e-task/,$(E2E_UNITS))

# Make owns case-level scheduling. Do not expose its jobserver pipes to LSP descendants: a server
# that survives cleanup would otherwise keep the pipes open and prevent the parent Make from exiting.
define E2E_TASK
.PHONY: e2e-task/$(1)
e2e-task/$(1):
	@cd "$(ROOT)" && env -u E2E_RESULTS_SUMMARY_OUTPUT -u MAKEFLAGS -u MFLAGS \
		E2E_RESULTS_OUTPUT="$(E2E_SHARD_DIR)/$(E2E_PHASE)-$(subst /,__,$(1)).json" \
		$(RUNNER) --suite "$(SUITE)" --phase "$(E2E_PHASE)" \
		$(if $(filter provision,$(E2E_PHASE)),--server "$(1)",--case "$(1)") \
		--quiet-summary
endef

$(foreach unit,$(E2E_UNITS),$(eval $(call E2E_TASK,$(unit))))

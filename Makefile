FNSQL_TEST_POSTGRES_PORT := 5433

all:
	set -e; \
	export FNSQL_TEST_POSTGRES_PORT=$(FNSQL_TEST_POSTGRES_PORT); \
	cargo test -- postgres::tests::docker_up --ignored; \
	sleep 2; \
	cargo test --all-features; \
	cargo run; \
	cargo test -- postgres::tests::docker_down --ignored;

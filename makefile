FORCE:

commit: FORCE lint tests build
	git add .
	git commit -a

prod: FORCE commit
	git push

run: FORCE clean
	cargo run

tests: FORCE
	cargo test

lint: FORCE
	cargo clippy --all-targets --color always  --allow-dirty --allow-staged --fix -- -D warnings -D clippy::pedantic -D clippy::nursery -D clippy::unwrap_used -D clippy::expect_used

build: FORCE lint tests
	cargo build

build_release: FORCE lint tests
	cargo build --release

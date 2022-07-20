install: build
	cargo install --path .

build:
	cargo build --release

cli-test:
	./target/release/la_template_rs; true
	./target/release/la_template_rs --help
	./target/release/la_template_rs -t tests/first_sub.t.txt -v tests/first_sub.json
	./target/release/la_template_rs --template tests/hello_report.t.txt --var-json tests/hello_report.json
	./target/release/la_template_rs -t tests/first_sub.t.txt -v tests/hello_report.json; true

.PHONY: install build cli-test

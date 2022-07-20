install: build
	cargo install --path .

build:
	cargo build --release

cli-test:
	./target/release/la_template_base; true
	./target/release/la_template_base --help
	./target/release/la_template_base -t tests/first_sub.t.txt -v tests/first_sub.json
	./target/release/la_template_base --template tests/hello_report.t.txt --var-json tests/hello_report.json
	./target/release/la_template_base -t tests/first_sub.t.txt -v tests/hello_report.json; true
	echo "ok"

.PHONY: install build cli-test

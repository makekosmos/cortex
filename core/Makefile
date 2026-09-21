.PHONY: run run-args shell clean setup

setup:
	mkdir -p .pi .npm-global

run: setup
	docker compose run --rm pi

run-args: setup
	docker compose run --rm pi $(args)

shell: setup
	docker compose run --rm --entrypoint /bin/bash pi

clean:
	docker compose down

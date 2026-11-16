set export

build board="gamma":
  cd firmware && cargo build --locked --release --features=bitaxe-{{board}}

test:
  cargo test --locked

web:
  python scripts/build-axeos.py

web-demo:
  python scripts/build-axeos.py --demo

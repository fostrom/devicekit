group "default" {
  targets = ["build-alpine"]
}

target "build-alpine" {
  context = "."
  dockerfile = "dockerfiles/build-alpine.Dockerfile"
  tags = ["devicekit/build-alpine:latest"]
}

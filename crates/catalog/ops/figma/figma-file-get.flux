op figma-file-get(file_key: String) -> Any
  description "Get a Figma file's document tree, components and styles by file key. Does not include comments or rendered images. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/files/{file_key}")
  response = http.request(method: "GET", url)
  return response

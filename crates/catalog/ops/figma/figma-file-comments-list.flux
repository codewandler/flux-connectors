op figma-file-comments-list(file_key: String) -> Any
  description "List a file's comments: who wrote each one, when, its text, what it is attached to, and whether it has been resolved. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/files/{file_key}/comments")
  response = http.request(method: "GET", url)
  return response

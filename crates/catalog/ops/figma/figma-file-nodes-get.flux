op figma-file-nodes-get(file_key: String, ids: String) -> Any
  description "Get one or more specific nodes from a file by id, without walking the whole document tree. Use this to read a particular frame or layer instead of downloading the entire file. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/err` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.figma.com"
  url = fmt("{base}/v1/files/{file_key}/nodes")
  response = http.request(method: "GET", query: { ids }, url)
  return response

op box-file-get(file_id: String) -> Any
  description "Get one file's metadata: name, size, timestamps and parent folder. This does NOT return the file's content — Box serves file content from a signed, time-limited URL reached only through a 302 redirect this connector does not follow, so file content is out of scope entirely. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.box.com"
  url = fmt("{base}/2.0/files/{file_id}")
  response = http.request(method: "GET", url)
  return response

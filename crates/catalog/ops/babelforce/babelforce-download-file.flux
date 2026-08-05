op babelforce-download-file(id: String) -> Any
  description "Download a file"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/{id}/download")
  response = http.request(method: "GET", url)
  return response

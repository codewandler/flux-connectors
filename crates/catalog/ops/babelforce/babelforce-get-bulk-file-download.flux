op babelforce-get-bulk-file-download(ids: String) -> Any
  description "Download files as a ZIP"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/bulk/download")
  response = http.request(method: "GET", query: { ids }, url)
  return response

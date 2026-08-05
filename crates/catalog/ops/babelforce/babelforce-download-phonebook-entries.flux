op babelforce-download-phonebook-entries -> Any
  description "Download phonebook entries (CSV)"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/phonebook/bulk")
  response = http.request(method: "GET", url)
  return response

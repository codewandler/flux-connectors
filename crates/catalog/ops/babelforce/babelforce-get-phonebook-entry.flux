op babelforce-get-phonebook-entry(id: String) -> Any
  description "Get a phonebook entry"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/phonebook/{id}")
  response = http.request(method: "GET", url)
  return response

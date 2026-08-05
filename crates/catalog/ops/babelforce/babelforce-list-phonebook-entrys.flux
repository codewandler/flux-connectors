op babelforce-list-phonebook-entrys(page: Number, max: Number) -> Any
  description "List phonebook entries"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/phonebook")
  response = http.request(method: "GET", query: { max, page }, url)
  return response

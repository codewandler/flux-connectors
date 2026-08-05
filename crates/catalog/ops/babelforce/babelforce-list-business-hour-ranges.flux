op babelforce-list-business-hour-ranges(id: String) -> Any
  description "List a profile's time ranges"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours/{id}/ranges")
  response = http.request(method: "GET", url)
  return response

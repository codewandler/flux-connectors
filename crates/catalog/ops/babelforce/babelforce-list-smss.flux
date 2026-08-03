op babelforce-list-smss(page: Number, max: Number) -> Any
  description "List SMS messages"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sms")
  response = http.request(method: "GET", query: { max, page }, url)
  return response

op babelforce-report-sms(page: Number, max: Number) -> Any
  description "Get an SMS report"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sms/reporting")
  response = http.request(method: "GET", query: { max, page }, url)
  return response

op babelforce-list-babeldesk-widgets(page: Number, max: Number) -> Any
  description "Get a List of all BabeldeskWidgets"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/widgets")
  response = http.request(method: "GET", query: { max, page }, url)
  return response

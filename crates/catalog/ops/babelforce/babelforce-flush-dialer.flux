op babelforce-flush-dialer(id: String, all: Bool) -> Any
  description "Flush dialer tasks"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dialer/flush")
  response = http.request(method: "GET", query: { all, id }, url)
  return response

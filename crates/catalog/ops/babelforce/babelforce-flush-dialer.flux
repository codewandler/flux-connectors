op babelforce-flush-dialer(id: String, all: Bool) -> Any
  description "Flush dialer tasks"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dialer/flush")
  sep = "?"
  when id
    url = fmt("{url}{sep}id={id}")
    sep = "&"
  when all
    url = fmt("{url}{sep}all={all}")
  response = http.request(method: "GET", url)
  return response

op babelforce-list-timezones(q: String, max: Number) -> Any
  description "List timezones"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/data/timezones")
  sep = "?"
  when q
    url = fmt("{url}{sep}q={q}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
  response = http.request(method: "GET", url)
  return response

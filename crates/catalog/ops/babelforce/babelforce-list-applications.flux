op babelforce-list-applications(page: Number, max: Number) -> Any
  description "List applications"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
  response = http.request(method: "GET", url)
  return response

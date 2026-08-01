op babelforce-list-dashboards(page: Number, max: Number, q: String, uuid: Any, sort: String, order: String) -> Any
  description "List dashboards"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dashboards")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when q
    url = fmt("{url}{sep}q={q}")
    sep = "&"
  when uuid
    url = fmt("{url}{sep}uuid={uuid}")
    sep = "&"
  when sort
    url = fmt("{url}{sep}sort={sort}")
    sep = "&"
  when order
    url = fmt("{url}{sep}order={order}")
  response = http.request(method: "GET", url)
  return response

op babelforce-list-files(page: Number, max: Number, sort: String, order: String, type: String, state: String, filename: String, q: String) -> Any
  description "List files"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when sort
    url = fmt("{url}{sep}sort={sort}")
    sep = "&"
  when order
    url = fmt("{url}{sep}order={order}")
    sep = "&"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when state
    url = fmt("{url}{sep}state={state}")
    sep = "&"
  when filename
    url = fmt("{url}{sep}filename={filename}")
    sep = "&"
  when q
    url = fmt("{url}{sep}q={q}")
  response = http.request(method: "GET", url)
  return response

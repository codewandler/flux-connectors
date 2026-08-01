op babelforce-list-conversations(page: Number, max: Number, phone: String, state: String) -> Any
  description "List conversations"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conversations")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when phone
    url = fmt("{url}{sep}phone={phone}")
    sep = "&"
  when state
    url = fmt("{url}{sep}state={state}")
  response = http.request(method: "GET", url)
  return response

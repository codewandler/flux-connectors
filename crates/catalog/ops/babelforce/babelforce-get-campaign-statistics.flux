op babelforce-get-campaign-statistics(id: String, from: Number, to: Number) -> Any
  description "Get campaign statistics"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/statistics")
  sep = "?"
  when from
    url = fmt("{url}{sep}from={from}")
    sep = "&"
  when to
    url = fmt("{url}{sep}to={to}")
  response = http.request(method: "GET", url)
  return response

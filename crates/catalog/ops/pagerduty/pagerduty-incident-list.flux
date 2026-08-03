op pagerduty-incident-list(limit: Number, offset: Number) -> Any
  description "List the incidents this key can see, across every service it has access to. Returns one page; read `more` to learn whether another page exists, and page by adding `limit` to `offset`. No status, service, team or time filter is offered by this connector — see pagerduty-incident-get to inspect one incident in full"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/incidents")
  Accept = "application/vnd.pagerduty+json;version=2"
  response = http.request(headers: { Accept }, method: "GET", query: { limit, offset }, url)
  return response

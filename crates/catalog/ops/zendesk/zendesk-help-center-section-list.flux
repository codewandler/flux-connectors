op zendesk-help-center-section-list -> Any
  description "List Help Center sections across all categories"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/sections")
  response = http.request(method: "GET", url)
  return response

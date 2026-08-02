op zendesk-help-center-category-list -> Any
  description "List Help Center categories; this zero-argument read also verifies that the shared Zendesk account and credential can reach the knowledge base"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/help_center/categories")
  response = http.request(method: "GET", url)
  return response

op zendesk-organization-show(organization_id: Number) -> Any
  description "Get one Zendesk organization by numeric id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/organizations/{organization_id}")
  response = http.request(method: "GET", url)
  return response

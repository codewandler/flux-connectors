op webflow-collection-list(site_id: String) -> Any
  description "List the CMS collections defined on a site, with each collection's id, display name and slug. Does not include a collection's fields — call webflow-collection-get for that. Returns Webflow's first page only; this connector declares no offset or limit parameter"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/sites/{site_id}/collections")
  response = http.request(method: "GET", url)
  return response

op webflow-site-list -> Any
  description "List the sites this token can see, with each site's id, display name, hosted subdomain and last-published time. The `id` returned here is what every other operation in this connector needs as `site_id`"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/sites")
  response = http.request(method: "GET", url)
  return response

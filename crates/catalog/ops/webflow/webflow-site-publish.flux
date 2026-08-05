op webflow-site-publish(site_id: String) -> Any
  description "Publish a site's currently staged changes to its live, public domains — every connected custom domain and the Webflow-hosted subdomain. This has immediate public effect: whatever is staged goes live for every visitor as soon as this call returns, with no separate confirmation step. Takes no body — Webflow's optional custom-domain selector is itself an array this connector cannot express (C-185), so this always publishes to every connected domain rather than a caller-chosen subset"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/sites/{site_id}/publish")
  response = http.request(method: "POST", url)
  return response

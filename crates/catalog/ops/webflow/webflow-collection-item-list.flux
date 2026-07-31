op webflow-collection-item-list(collection_id: String) -> Any
  description "List the items in a collection, in Webflow's default order. Each item's fieldData is a flat object keyed by that collection's own field slugs — call webflow-collection-get to discover them; this connector cannot type fieldData beyond that, because it is defined per-tenant, per-collection. Returns Webflow's first page only; this connector declares no offset or limit parameter"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/collections/{collection_id}/items")
  response = http.request(method: "GET", url)
  return response

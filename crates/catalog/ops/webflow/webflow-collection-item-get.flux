op webflow-collection-item-get(collection_id: String, item_id: String) -> Any
  description "Get one item from a collection. Its fieldData is a flat object keyed by that collection's own field slugs — call webflow-collection-get to discover them; this connector cannot type fieldData beyond that, because it is defined per-tenant, per-collection"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/collections/{collection_id}/items/{item_id}")
  response = http.request(method: "GET", url)
  return response

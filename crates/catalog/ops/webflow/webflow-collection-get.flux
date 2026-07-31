op webflow-collection-get(collection_id: String) -> Any
  description "Get a collection's own schema: its display name, slug, and the full list of fields the site owner defined for it — each field's id, slug, display name, type, and whether it is required. This is the honest way to learn what an item's fieldData will contain, since that shape is tenant-defined and unknowable at compile time; this connector cannot type fieldData any more precisely than this lets a caller discover it"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.webflow.com/v2"
  url = fmt("{base}/collections/{collection_id}")
  response = http.request(method: "GET", url)
  return response

op confluence-space-pages(id: String) -> Any
  description "List the pages in one space, with the id, title, parent and version of each — the way to discover what a space contains and to resolve a page title to the id the other operations take. **Page bodies are not returned**: the content of each page is empty here, because selecting a body format needs a query parameter this connector cannot send. Returns only Confluence's first page of 25 results; a larger space is truncated"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/spaces/{id}/pages")
  response = http.request(method: "GET", url)
  return response

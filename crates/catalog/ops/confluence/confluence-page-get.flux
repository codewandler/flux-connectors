op confluence-page-get(id: String) -> Any
  description "Read one page's metadata by id — title, space, parent, author, creation time, current version number and the link to open it in Confluence. **The page body is not returned**: selecting a content format needs a query parameter this connector cannot send, so the `body` field comes back empty regardless of whether the page has content. Use this to resolve a page id to its title and URL, not to read what the page says"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/pages/{id}")
  response = http.request(method: "GET", url)
  return response

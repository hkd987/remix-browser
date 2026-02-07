# Project overview 
We are building a better chrome mcp cdp connection and remix-browser is that tool

## Web Searches
 - Never include the date or year in your web searches, its weird and doesn't help

## When planning
 - You look first at documentation from the source
 - When looking at documentation you will get 3-5 sources only. That should be plenty, if you need to lookup more docs ask

## Work as a team
When asked to do work always spin up multiple agents and work as a team to get the job done as fast as possible. High quality code written fast as a team is the goal here. Working together and sharing when needed.

 ## Code Quality
 - Always unit test code that we write ALWAYS. 
 - Always ensure the code is linted and or type checked
 - Always ensure the project builds
 - Always ensure the code is DRY
 
## When Planning or testing
 - Always see how you can validate a change you have made to ensure its correct
     - Examples
         - When asked to optimize code or make code faster, always have a performance benchmark you can run before and after
         - When asked to write a new feature, or extend code write supporting unit test if needed first then add the new feature then add more unit test as needed
 - If you are unsure about an ask, always use the AskUserQuestion tool and get the answers your need

## Browser Automation
When the user asks to use Chrome, use the browser, browse a website, open a URL,
take a screenshot, test a web app, fill forms, scrape content, debug a UI, inspect
the DOM, run JavaScript in the browser, or do anything browser-related — use the
remix-browser MCP tools (`mcp__remix-browser__*`). Always start with `navigate`.
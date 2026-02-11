# Edge Cases

## Empty Section Followed by Empty Section

##

## Another Empty Heading

## Unicode Headings

Content between unicode headings.

## 中文标题

Chinese heading content.

## Ünïcödé

Accented characters heading content.

## Emoji Heading

Emoji in heading content.

## Very Long Line

This is a very long line that goes on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on until it exceeds five hundred characters total which is the threshold we want to test for performance and rendering edge cases in various markdown processors and language servers.

## Deeply Nested Lists

- Level 1
  - Level 2
    - Level 3
      - Level 4
        - Level 5
          - Level 6
            - Level 7
              - Level 8
                - Level 9
                  - Level 10
                    - Level 11

## Code Block with Markdown Syntax

```markdown
# This Heading Should Not Be Indexed

## Neither Should This

[[this-link-should-not-resolve]]

<agent>not a real tag</agent>
```

## Inline Code with Special Content

The heading `## Not A Heading` in inline code should be ignored.

A wiki link in code: `[[not-a-link]]` should also be ignored.

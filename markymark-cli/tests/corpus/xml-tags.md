# XML Tags Test

## Paired Tags

<agent>
This is content inside an agent tag.
</agent>

<task>
A simple task element.
</task>

<agent>
Another agent tag instance.
</agent>

## Self-Closing Tags

<step />

<checkpoint />

## Nested Tags

<agent>
  <task>
    Nested task inside agent.
  </task>
  <step />
</agent>

## Tags with Attributes

<agent name="test-agent" priority="1">
Content with attributes on the tag.
</agent>

<task type="review" status="pending">
Attributed task element.
</task>

## Unclosed Tag

<broken>
This tag is intentionally unclosed for edge case testing.

## Multiple Instances

<step />
<step />
<task>Third task instance</task>

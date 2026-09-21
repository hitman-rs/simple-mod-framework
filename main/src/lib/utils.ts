import DOMPurify from "dompurify"
import { Duration, type DurationLikeObject } from "luxon"

export const sanitise = (html: string) => DOMPurify.sanitize(html, { ALLOWED_TAGS: ["b", "i", "em", "strong", "h1", "h2", "h3", "ul", "ol", "li", "p"], ALLOWED_ATTR: [] })

export const sanitiseInline = (html: string) => DOMPurify.sanitize(html, { ALLOWED_TAGS: ["b", "i", "em", "strong", "ul", "ol", "li", "p"], ALLOWED_ATTR: [] })

export function toHuman(dur: Duration, smallestUnit = "milliseconds"): string {
	const units = ["years", "months", "days", "hours", "minutes", "seconds", "milliseconds"]
	const smallestIdx = units.indexOf(smallestUnit)
	const entries = Object.entries(
		dur
			.shiftTo(...(units as (keyof DurationLikeObject)[]))
			.normalize()
			.toObject()
	).filter((val, idx) => val[1] > 0 && idx <= smallestIdx)
	const dur2 = Duration.fromObject(entries.length === 0 ? { [smallestUnit]: 0 } : Object.fromEntries(entries))
	return dur2.toHuman()
}

export function observe<T extends HTMLElement>(node: T, { callback, ...options }: { callback: (self: T, visible: boolean, ratio: number) => any } & IntersectionObserverInit) {
	const observer = new IntersectionObserver((entries) => {
		callback(node, entries[0].isIntersecting, entries[0].intersectionRatio)
	}, options)

	observer.observe(node)

	return {
		destroy() {
			observer.disconnect()
		}
	}
}

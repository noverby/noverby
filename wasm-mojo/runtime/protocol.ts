// Mutation Buffer Protocol — JS side
//
// Decodes binary-encoded DOM mutations written by the Mojo MutationWriter.
// All multi-byte integers are little-endian. Strings are length-prefixed UTF-8.

const decoder = new TextDecoder();

// ── Opcodes (must match src/bridge/protocol.mojo) ───────────────────────────

export const Op = {
	End: 0x00,
	AppendChildren: 0x01,
	AssignId: 0x02,
	CreatePlaceholder: 0x03,
	CreateTextNode: 0x04,
	LoadTemplate: 0x05,
	ReplaceWith: 0x06,
	ReplacePlaceholder: 0x07,
	InsertAfter: 0x08,
	InsertBefore: 0x09,
	SetAttribute: 0x0a,
	SetText: 0x0b,
	NewEventListener: 0x0c,
	RemoveEventListener: 0x0d,
	Remove: 0x0e,
	PushRoot: 0x0f,
} as const;

export type OpCode = (typeof Op)[keyof typeof Op];

// ── Mutation types ──────────────────────────────────────────────────────────

export interface MutationAppendChildren {
	op: typeof Op.AppendChildren;
	id: number;
	m: number;
}

export interface MutationAssignId {
	op: typeof Op.AssignId;
	path: Uint8Array;
	id: number;
}

export interface MutationCreatePlaceholder {
	op: typeof Op.CreatePlaceholder;
	id: number;
}

export interface MutationCreateTextNode {
	op: typeof Op.CreateTextNode;
	id: number;
	text: string;
}

export interface MutationLoadTemplate {
	op: typeof Op.LoadTemplate;
	tmplId: number;
	index: number;
	id: number;
}

export interface MutationReplaceWith {
	op: typeof Op.ReplaceWith;
	id: number;
	m: number;
}

export interface MutationReplacePlaceholder {
	op: typeof Op.ReplacePlaceholder;
	path: Uint8Array;
	m: number;
}

export interface MutationInsertAfter {
	op: typeof Op.InsertAfter;
	id: number;
	m: number;
}

export interface MutationInsertBefore {
	op: typeof Op.InsertBefore;
	id: number;
	m: number;
}

export interface MutationSetAttribute {
	op: typeof Op.SetAttribute;
	id: number;
	ns: number;
	name: string;
	value: string;
}

export interface MutationSetText {
	op: typeof Op.SetText;
	id: number;
	text: string;
}

export interface MutationNewEventListener {
	op: typeof Op.NewEventListener;
	id: number;
	name: string;
}

export interface MutationRemoveEventListener {
	op: typeof Op.RemoveEventListener;
	id: number;
	name: string;
}

export interface MutationRemove {
	op: typeof Op.Remove;
	id: number;
}

export interface MutationPushRoot {
	op: typeof Op.PushRoot;
	id: number;
}

export type Mutation =
	| MutationAppendChildren
	| MutationAssignId
	| MutationCreatePlaceholder
	| MutationCreateTextNode
	| MutationLoadTemplate
	| MutationReplaceWith
	| MutationReplacePlaceholder
	| MutationInsertAfter
	| MutationInsertBefore
	| MutationSetAttribute
	| MutationSetText
	| MutationNewEventListener
	| MutationRemoveEventListener
	| MutationRemove
	| MutationPushRoot;

// ── MutationReader ──────────────────────────────────────────────────────────

/**
 * Reads binary-encoded mutations from an ArrayBuffer (or a region of WASM
 * linear memory).  Call `next()` repeatedly to decode one mutation at a time,
 * or `readAll()` to drain the buffer.
 */
export class MutationReader {
	private view: DataView;
	private bytes: Uint8Array;
	private offset: number;
	private end: number;

	/**
	 * @param buffer - The underlying ArrayBuffer (typically WASM memory).
	 * @param byteOffset - Start of the mutation data within `buffer`.
	 * @param byteLength - Number of bytes of mutation data.
	 */
	constructor(buffer: ArrayBuffer, byteOffset: number, byteLength: number) {
		this.view = new DataView(buffer, byteOffset, byteLength);
		this.bytes = new Uint8Array(buffer, byteOffset, byteLength);
		this.offset = 0;
		this.end = byteLength;
	}

	/** Bytes consumed so far. */
	get position(): number {
		return this.offset;
	}

	/** Bytes remaining. */
	get remaining(): number {
		return this.end - this.offset;
	}

	// ── Primitive decoders ────────────────────────────────────────────

	private readU8(): number {
		const v = this.view.getUint8(this.offset);
		this.offset += 1;
		return v;
	}

	private readU16(): number {
		const v = this.view.getUint16(this.offset, true); // little-endian
		this.offset += 2;
		return v;
	}

	private readU32(): number {
		const v = this.view.getUint32(this.offset, true); // little-endian
		this.offset += 4;
		return v;
	}

	/** Read a u32-length-prefixed UTF-8 string. */
	private readStr(): string {
		const len = this.readU32();
		if (len === 0) return "";
		const slice = this.bytes.subarray(this.offset, this.offset + len);
		this.offset += len;
		return decoder.decode(slice);
	}

	/** Read a u16-length-prefixed UTF-8 string (for attribute/event names). */
	private readShortStr(): string {
		const len = this.readU16();
		if (len === 0) return "";
		const slice = this.bytes.subarray(this.offset, this.offset + len);
		this.offset += len;
		return decoder.decode(slice);
	}

	/** Read a u8-length-prefixed byte path. */
	private readPath(): Uint8Array {
		const len = this.readU8();
		const path = this.bytes.slice(this.offset, this.offset + len);
		this.offset += len;
		return path;
	}

	// ── Public API ────────────────────────────────────────────────────

	/**
	 * Decode the next mutation from the buffer.
	 * Returns `null` when the End sentinel is reached or the buffer is
	 * exhausted.
	 */
	next(): Mutation | null {
		if (this.offset >= this.end) return null;

		const op = this.readU8();

		switch (op) {
			case Op.End:
				return null;

			case Op.AppendChildren:
				return { op, id: this.readU32(), m: this.readU32() };

			case Op.AssignId:
				return { op, path: this.readPath(), id: this.readU32() };

			case Op.CreatePlaceholder:
				return { op, id: this.readU32() };

			case Op.CreateTextNode:
				return { op, id: this.readU32(), text: this.readStr() };

			case Op.LoadTemplate:
				return {
					op,
					tmplId: this.readU32(),
					index: this.readU32(),
					id: this.readU32(),
				};

			case Op.ReplaceWith:
				return { op, id: this.readU32(), m: this.readU32() };

			case Op.ReplacePlaceholder:
				return { op, path: this.readPath(), m: this.readU32() };

			case Op.InsertAfter:
				return { op, id: this.readU32(), m: this.readU32() };

			case Op.InsertBefore:
				return { op, id: this.readU32(), m: this.readU32() };

			case Op.SetAttribute: {
				const id = this.readU32();
				const ns = this.readU8();
				const name = this.readShortStr();
				const value = this.readStr();
				return { op, id, ns, name, value };
			}

			case Op.SetText:
				return { op, id: this.readU32(), text: this.readStr() };

			case Op.NewEventListener:
				return { op, id: this.readU32(), name: this.readShortStr() };

			case Op.RemoveEventListener:
				return { op, id: this.readU32(), name: this.readShortStr() };

			case Op.Remove:
				return { op, id: this.readU32() };

			case Op.PushRoot:
				return { op, id: this.readU32() };

			default:
				throw new Error(
					`Unknown mutation opcode 0x${op.toString(16).padStart(2, "0")} at offset ${this.offset - 1}`,
				);
		}
	}

	/**
	 * Read all mutations until End sentinel or buffer exhaustion.
	 * Returns an array of decoded mutations (the End sentinel itself is
	 * not included).
	 */
	readAll(): Mutation[] {
		const mutations: Mutation[] = [];
		for (;;) {
			const m = this.next();
			if (m === null) break;
			mutations.push(m);
		}
		return mutations;
	}
}

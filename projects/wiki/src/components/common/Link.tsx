import React, { type ForwardedRef } from "react";
import { type LinkProps, Link as RouterLink } from "react-router-dom";

type ComposedProps = LinkProps & React.AnchorHTMLAttributes<HTMLAnchorElement>;

const RawLink = (
	props: ComposedProps,
	ref: ForwardedRef<HTMLAnchorElement>,
) => {
	return <RouterLink {...props} ref={ref} />;
};

const Link = React.forwardRef<HTMLAnchorElement, ComposedProps>(RawLink);

export default Link;

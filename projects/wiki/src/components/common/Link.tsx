import NextLink, { type LinkProps as NextLinkProps } from "next/link";
import React, { type ForwardedRef } from "react";

type NextComposedProps = Omit<
	React.AnchorHTMLAttributes<HTMLAnchorElement>,
	"href"
> &
	NextLinkProps;

const RawLink = (
	props: NextComposedProps,
	ref: ForwardedRef<HTMLAnchorElement>,
) => {
	const { as, href, ...other } = props;

	return <NextLink href={href} as={as} {...other} ref={ref} />;
};

const Link = React.forwardRef<HTMLAnchorElement, NextComposedProps>(RawLink);

export default Link;

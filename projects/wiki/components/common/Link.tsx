import React, { ForwardedRef } from 'react';
import NextLink, { LinkProps as NextLinkProps } from 'next/link';

type NextComposedProps = Omit<
  React.AnchorHTMLAttributes<HTMLAnchorElement>,
  'href'
> &
  NextLinkProps;

const RawLink = (
  props: NextComposedProps,
  ref: ForwardedRef<HTMLAnchorElement>
) => {
  const { as, href, ...other } = props;

  return <NextLink href={href} as={as} {...other} ref={ref} />;
};

const Link = React.forwardRef<HTMLAnchorElement, NextComposedProps>(RawLink);

export default Link;

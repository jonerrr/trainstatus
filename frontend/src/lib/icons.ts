// subway icons from https://github.com/louh/mta-subway-bullets/tree/main/svg

const icons = [
	{
		name: '1',
		svg: `<g id="1">
        <circle cx="45" cy="45" r="45" style="fill:rgb(238,52,46);"/>
        <path d="M31.084,36.4388L31.084,30.1237C34.0137,29.9935 36.0645,29.7982 37.2363,29.5378C39.1026,29.1254 40.6217,28.3008 41.7936,27.0638C42.5966,26.2174 43.2042,25.089 43.6165,23.6784C43.8553,22.832 43.9746,22.2027 43.9746,21.7904L51.6895,21.7904L51.6895,68.9909L42.1842,68.9909L42.1842,36.4388L31.084,36.4388Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '2',
		svg: `<g id="2">
    <circle cx="45" cy="45" r="45" style="fill:rgb(238,52,46);"/>
    <path d="M30.7259,59.7135C32.0497,56.5668 35.1747,53.2357 40.1009,49.7201C44.3761,46.6602 47.143,44.4683 48.4017,43.1445C50.3331,41.0829 51.2988,38.826 51.2988,36.3737C51.2988,34.3772 50.7454,32.717 49.6387,31.3932C48.5319,30.0694 46.9477,29.4076 44.8861,29.4076C42.0649,29.4076 40.1443,30.4601 39.1243,32.5651C38.5384,33.7804 38.1912,35.7118 38.0827,38.3594L29.0658,38.3594C29.2177,34.3446 29.9447,31.1003 31.2467,28.6263C33.7207,23.9171 38.1152,21.5625 44.4303,21.5625C49.4217,21.5625 53.393,22.946 56.3444,25.7129C59.2958,28.4798 60.7715,32.1419 60.7715,36.6992C60.7715,40.1931 59.7298,43.2964 57.6465,46.0091C56.2793,47.8103 54.0332,49.8177 50.9082,52.0313L47.1973,54.668C44.8752,56.3173 43.2856,57.5109 42.4284,58.2487C41.5712,58.9865 40.8496,59.8438 40.2637,60.8203L60.8691,60.8203L60.8691,68.9909L28.5449,68.9909C28.6317,65.6055 29.3587,62.513 30.7259,59.7135Z" style="fill:white;fill-rule:nonzero;"/>
</g>`
	},
	{
		name: '3',
		svg: `<g id="3">
        <circle cx="45" cy="45" r="45" style="fill:rgb(238,52,46);"/>
        <path d="M37.334,54.5052C37.334,56.3932 37.6378,57.9557 38.2454,59.1927C39.3739,61.4714 41.4247,62.6107 44.3978,62.6107C46.2207,62.6107 47.8103,61.9868 49.1667,60.7389C50.523,59.4911 51.2012,57.6953 51.2012,55.3516C51.2012,52.2483 49.9425,50.1758 47.4251,49.1341C45.9928,48.5482 43.7359,48.2552 40.6543,48.2552L40.6543,41.6146C43.6708,41.5712 45.7758,41.2782 46.9694,40.7357C49.031,39.8242 50.0618,37.9796 50.0618,35.2018C50.0618,33.4006 49.5356,31.9358 48.4831,30.8073C47.4306,29.6788 45.9494,29.1146 44.0397,29.1146C41.8479,29.1146 40.2365,29.809 39.2057,31.1979C38.1749,32.5868 37.6812,34.4423 37.7246,36.7643L29.0658,36.7643C29.1526,34.4206 29.554,32.1962 30.2702,30.0911C31.0297,28.2465 32.2233,26.543 33.8509,24.9805C35.0662,23.8737 36.5093,23.0273 38.1803,22.4414C39.8513,21.8555 41.9021,21.5625 44.3327,21.5625C48.8466,21.5625 52.487,22.7289 55.2539,25.0618C58.0208,27.3947 59.4043,30.5252 59.4043,34.4531C59.4043,37.2309 58.5796,39.5747 56.9303,41.4844C55.8887,42.678 54.8036,43.4918 53.6751,43.9258C54.5215,43.9258 55.7368,44.6528 57.321,46.1068C59.6864,48.2986 60.8691,51.2934 60.8691,55.0911C60.8691,59.0842 59.4857,62.5944 56.7188,65.6217C53.9518,68.6491 49.8557,70.1628 44.4303,70.1628C37.7463,70.1628 33.1022,67.9818 30.498,63.6198C29.1309,61.2977 28.3713,58.2595 28.2194,54.5052L37.334,54.5052Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '4',
		svg: `<g id="4">
        <circle cx="45" cy="45" r="45" style="fill:rgb(0,147,59);"/>
        <path d="M61.3249,58.8346L55.9863,58.8346L55.9863,68.9909L46.9043,68.9909L46.9043,58.8346L28.2194,58.8346L28.2194,50.7292L45.5697,22.0833L55.9863,22.0833L55.9863,51.5755L61.3249,51.5755L61.3249,58.8346ZM46.9043,51.5755L46.9043,31.1979L35.0879,51.5755L46.9043,51.5755Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '5',
		svg: `<g id="5">
        <circle cx="45" cy="45" r="45" style="fill:rgb(0,147,59);"/>
        <path d="M37.4642,56.3607C37.8331,58.3789 38.5384,59.936 39.5801,61.0319C40.6217,62.1278 42.1408,62.6758 44.1374,62.6758C46.4377,62.6758 48.1901,61.8674 49.3945,60.2507C50.599,58.6339 51.2012,56.5994 51.2012,54.1471C51.2012,51.7383 50.6369,49.7038 49.5085,48.0436C48.38,46.3835 46.6222,45.5534 44.235,45.5534C43.1066,45.5534 42.13,45.6944 41.3053,45.9766C39.8513,46.4974 38.7554,47.4631 38.0176,48.8737L29.6842,48.4831L33.0046,22.4089L59.0137,22.4089L59.0137,30.2865L39.7103,30.2865L38.0176,40.6055C39.4499,39.6723 40.5675,39.0538 41.3704,38.75C42.7159,38.2509 44.3544,38.0013 46.2858,38.0013C50.1921,38.0013 53.5992,39.3142 56.5072,41.9401C59.4151,44.566 60.8691,48.3854 60.8691,53.3984C60.8691,57.7604 59.4694,61.6558 56.6699,65.0846C53.8704,68.5135 49.6821,70.2279 44.1048,70.2279C39.6126,70.2279 35.9234,69.0234 33.0371,66.6146C30.1508,64.2057 28.5449,60.7878 28.2194,56.3607L37.4642,56.3607Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '6',
		svg: `<g id="6">
        <circle cx="45" cy="45" r="45" style="fill:rgb(0,147,59);"/>
        <path d="M39.9056,60.2669C41.2728,61.8728 43.0089,62.6758 45.1139,62.6758C47.1756,62.6758 48.7977,61.9 49.9805,60.3483C51.1632,58.7967 51.7546,56.7839 51.7546,54.3099C51.7546,51.5538 51.0818,49.4434 49.7363,47.9785C48.3908,46.5137 46.7415,45.7813 44.7884,45.7813C43.2042,45.7813 41.8045,46.2587 40.5892,47.2135C38.7663,48.6241 37.8548,50.9028 37.8548,54.0495C37.8548,56.5885 38.5384,58.661 39.9056,60.2669ZM50.9082,33.6719C50.9082,32.9123 50.6152,32.0768 50.0293,31.1654C49.031,29.6897 47.5228,28.9518 45.5046,28.9518C42.4881,28.9518 40.3396,30.6445 39.0592,34.0299C38.3648,35.8963 37.8874,38.6523 37.627,42.2982C38.7771,40.931 40.1118,39.9327 41.6309,39.3034C43.15,38.674 44.8861,38.3594 46.8392,38.3594C51.0276,38.3594 54.4618,39.7808 57.1419,42.6237C59.822,45.4666 61.1621,49.1016 61.1621,53.5286C61.1621,57.934 59.8492,61.8186 57.2233,65.1823C54.5974,68.546 50.5176,70.2279 44.9837,70.2279C39.0375,70.2279 34.6539,67.7431 31.8327,62.7734C29.6408,58.8889 28.5449,53.8759 28.5449,47.7344C28.5449,44.1319 28.6968,41.2023 29.0007,38.9453C29.5432,34.9306 30.5957,31.5885 32.1582,28.9193C33.5037,26.6406 35.2669,24.8069 37.4479,23.418C39.6289,22.0291 42.2385,21.3346 45.2767,21.3346C49.6604,21.3346 53.1543,22.4577 55.7585,24.7038C58.3626,26.9499 59.8275,29.9392 60.153,33.6719L50.9082,33.6719Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '7',
		svg: `<g id="7">
        <circle cx="45" cy="45" r="45" style="fill:rgb(185,51,174);"/>
        <path d="M61.6178,29.668C60.2289,31.0352 58.2975,33.4711 55.8236,36.9759C53.3496,40.4807 51.2771,44.0994 49.6061,47.832C48.2823,50.7617 47.0888,54.3424 46.0254,58.5742C44.962,62.806 44.4303,66.2782 44.4303,68.9909L34.7949,68.9909C35.077,60.5273 37.8548,51.7274 43.1283,42.5911C46.5354,36.9271 49.3891,32.9774 51.6895,30.7422L28.1543,30.7422L28.2845,22.4089L61.6178,22.4089L61.6178,29.668Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'A',
		svg: `<g id="A">
        <circle cx="45" cy="45" r="45" style="fill:rgb(40,82,173);"/>
        <path d="M39.1243,50.8594L51.2988,50.8594L45.3092,31.9792L39.1243,50.8594ZM39.7428,21.0091L51.071,21.0091L68.0632,68.9909L57.1908,68.9909L54.0983,59.1276L36.4225,59.1276L33.1022,68.9909L22.6204,68.9909L39.7428,21.0091Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'B',
		svg: `<g id="B">
        <circle cx="45" cy="45" r="45" style="fill:rgb(255,98,25);"/>
        <path d="M35.7389,29.3424L35.7389,39.9219L47.5228,39.9219C49.6278,39.9219 51.3368,39.5258 52.6497,38.7337C53.9627,37.9416 54.6191,36.5365 54.6191,34.5182C54.6191,32.283 53.7511,30.8073 52.015,30.0911C50.5176,29.592 48.6079,29.3424 46.2858,29.3424L35.7389,29.3424ZM35.7389,47.8646L35.7389,60.6576L47.5228,60.6576C49.6278,60.6576 51.2663,60.3754 52.4382,59.8112C54.5649,58.7695 55.6283,56.773 55.6283,53.8216C55.6283,51.326 54.5974,49.6115 52.5358,48.6784C51.3856,48.1576 49.7689,47.8863 47.6855,47.8646L35.7389,47.8646ZM61.9759,26.1198C63.4516,28.1597 64.1895,30.6011 64.1895,33.444C64.1895,36.3737 63.4516,38.7283 61.9759,40.5078C61.1513,41.5061 59.936,42.4175 58.3301,43.2422C60.7823,44.1319 62.6324,45.5425 63.8802,47.474C65.128,49.4054 65.752,51.7491 65.752,54.5052C65.752,57.3481 65.0358,59.898 63.6035,62.1549C62.6921,63.6523 61.5527,64.911 60.1855,65.931C58.6447,67.1029 56.8273,67.9058 54.7331,68.3398C52.6389,68.7739 50.3657,68.9909 47.9134,68.9909L26.1686,68.9909L26.1686,21.0091L49.4759,21.0091C55.357,21.0959 59.5237,22.7995 61.9759,26.1198Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'C',
		svg: `<g id="C">
        <circle cx="45" cy="45" r="45" style="fill:rgb(40,82,173);"/>
        <path d="M30.6608,26.0221C34.5671,22.0725 39.5367,20.0977 45.5697,20.0977C53.6426,20.0977 59.5454,22.7452 63.278,28.0404C65.3396,31.0135 66.4464,33.9974 66.5983,36.9922L56.5723,36.9922C55.9212,34.6918 55.0857,32.9557 54.0658,31.7839C52.2428,29.7005 49.541,28.6589 45.9603,28.6589C42.3145,28.6589 39.439,30.1291 37.334,33.0697C35.2289,36.0102 34.1764,40.1714 34.1764,45.5534C34.1764,50.9353 35.2886,54.9664 37.513,57.6465C39.7374,60.3266 42.564,61.6667 45.9928,61.6667C49.5085,61.6667 52.1886,60.5165 54.0332,58.2161C55.0532,56.9792 55.8995,55.1237 56.5723,52.6497L66.5007,52.6497C65.6326,57.8798 63.4136,62.1332 59.8438,65.4102C56.2739,68.6871 51.7003,70.3255 46.123,70.3255C39.222,70.3255 33.7967,68.112 29.847,63.6849C25.8974,59.2361 23.9225,53.138 23.9225,45.3906C23.9225,37.0139 26.1686,30.5577 30.6608,26.0221Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'D',
		svg: `<g id="D">
        <circle cx="45" cy="45" r="45" style="fill:rgb(255,98,25);"/>
        <path d="M35.7389,29.3424L35.7389,60.6576L44.9837,60.6576C49.7146,60.6576 53.0132,58.3247 54.8796,53.6589C55.8995,51.0981 56.4095,48.049 56.4095,44.5117C56.4095,39.6289 55.6445,35.88 54.1146,33.265C52.5846,30.65 49.541,29.3424 44.9837,29.3424L35.7389,29.3424ZM54.0983,22.0508C57.462,23.1576 60.1855,25.1866 62.2689,28.138C63.9399,30.5252 65.0792,33.1076 65.6868,35.8854C66.2945,38.6632 66.5983,41.3108 66.5983,43.8281C66.5983,50.2083 65.3179,55.612 62.7572,60.0391C59.2849,66.0069 53.9247,68.9909 46.6764,68.9909L26.0059,68.9909L26.0059,21.0091L46.6764,21.0091C49.6495,21.0525 52.1235,21.3997 54.0983,22.0508Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'E',
		svg: `<g id="E">
        <circle cx="45" cy="45" r="45" style="fill:rgb(40,82,173);"/>
        <path d="M63.3268,29.5052L37.9362,29.5052L37.9362,39.694L61.2435,39.694L61.2435,48.0273L37.9362,48.0273L37.9362,60.3646L64.4987,60.3646L64.4987,68.9909L28.138,68.9909L28.138,21.0091L63.3268,21.0091L63.3268,29.5052Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'F',
		svg: `<g id="F">
        <circle cx="45" cy="45" r="45" style="fill:rgb(255,98,25);"/>
        <path d="M29.7168,21.0742L63.7337,21.0742L63.7337,29.5052L39.6777,29.5052L39.6777,40.5404L60.7389,40.5404L60.7389,48.8737L39.6777,48.8737L39.6777,68.9909L29.7168,68.9909L29.7168,21.0742Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'G',
		svg: `<g id="G">
        <circle cx="45" cy="45" r="45" style="fill:rgb(109,190,69);"/>
        <path d="M56.4095,35.9831C55.65,32.7062 53.7945,30.4167 50.8431,29.1146C49.1938,28.3984 47.36,28.0404 45.3418,28.0404C41.4789,28.0404 38.3051,29.4998 35.8203,32.4186C33.3355,35.3375 32.0931,39.7266 32.0931,45.5859C32.0931,51.4887 33.4386,55.6662 36.1296,58.1185C38.8205,60.5707 41.8804,61.7969 45.3092,61.7969C48.673,61.7969 51.429,60.8257 53.5775,58.8835C55.7259,56.9412 57.0497,54.3967 57.5488,51.25L46.4486,51.25L46.4486,43.2422L66.4355,43.2422L66.4355,68.9909L59.7949,68.9909L58.7858,63.0013C56.8544,65.2799 55.1183,66.8859 53.5775,67.819C50.9299,69.4466 47.6747,70.2604 43.8118,70.2604C37.4533,70.2604 32.245,68.0577 28.1868,63.6523C23.9551,59.2253 21.8392,53.1706 21.8392,45.4883C21.8392,37.7192 23.9768,31.4909 28.252,26.8034C32.5271,22.1159 38.1803,19.7721 45.2116,19.7721C51.3097,19.7721 56.2088,21.3184 59.9089,24.4108C63.6089,27.5033 65.7303,31.3607 66.2728,35.9831L56.4095,35.9831Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'J',
		svg: `<g id="J">
        <circle cx="45" cy="45" r="45" style="fill:rgb(153,100,51);"/>
        <path d="M37.3991,50.7943L37.3991,51.901C37.4859,55.612 37.8928,58.2216 38.6198,59.7298C39.3468,61.2381 40.9039,61.9922 43.291,61.9922C45.6565,61.9922 47.219,61.1675 47.9785,59.5182C48.4342,58.5417 48.6621,56.8924 48.6621,54.5703L48.6621,21.0091L58.6882,21.0091L58.6882,54.4076C58.6882,58.4874 57.9829,61.7209 56.5723,64.1081C54.1851,68.1445 49.8774,70.1628 43.6491,70.1628C37.4208,70.1628 33.2433,68.5297 31.1165,65.2637C28.9898,61.9976 27.9264,57.5434 27.9264,51.901L27.9264,50.7943L37.3991,50.7943Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'L',
		svg: `<g id="L">
        <circle cx="45" cy="45" r="45" style="fill:rgb(167,169,172);"/>
        <path d="M29.7168,21.0091L39.7428,21.0091L39.7428,60.3646L63.5059,60.3646L63.5059,68.9909L29.7168,68.9909L29.7168,21.0091Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'M',
		svg: `<g id="M">
        <circle cx="45" cy="45" r="45" style="fill:rgb(255,98,25);"/>
        <path d="M54.1146,21.0091L68.5352,21.0091L68.5352,68.9909L59.1927,68.9909L59.1927,36.5365C59.1927,35.6033 59.2036,34.2958 59.2253,32.6139C59.247,30.9321 59.2578,29.6354 59.2578,28.724L50.1758,68.9909L40.4427,68.9909L31.4258,28.724C31.4258,29.6354 31.4366,30.9321 31.4583,32.6139C31.48,34.2958 31.4909,35.6033 31.4909,36.5365L31.4909,68.9909L22.1484,68.9909L22.1484,21.0091L36.7318,21.0091L45.4557,58.737L54.1146,21.0091Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'N',
		svg: `<g id="N">
        <circle cx="45" cy="45" r="45" style="fill:rgb(252,204,10);"/>
        <path d="M25.8431,21.0091L36.3574,21.0091L55.4655,54.5052L55.4655,21.0091L64.8079,21.0091L64.8079,68.9909L54.7819,68.9909L35.1855,34.9089L35.1855,68.9909L25.8431,68.9909L25.8431,21.0091Z" style="fill:black;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'Q',
		svg: `<g id="Q">
        <circle cx="45" cy="45" r="45" style="fill:rgb(252,204,10);"/>
        <path d="M48.597,61.4063C49.1829,61.2543 49.9316,60.9831 50.8431,60.5924L45.9928,55.9701L51.1686,50.5664L56.0189,55.1888C56.7784,53.6263 57.3101,52.2591 57.6139,51.0872C58.0914,49.3294 58.3301,47.2786 58.3301,44.9349C58.3301,39.553 57.2287,35.3917 55.026,32.4512C52.8234,29.5106 49.6061,28.0404 45.3743,28.0404C41.403,28.0404 38.2346,29.451 35.8691,32.2721C33.5037,35.0933 32.321,39.3142 32.321,44.9349C32.321,51.5104 34.0137,56.2196 37.3991,59.0625C39.5909,60.9071 42.2168,61.8294 45.2767,61.8294C46.4269,61.8294 47.5336,61.6884 48.597,61.4063ZM66.7936,54.9609C65.9473,57.717 64.6994,60.0065 63.0501,61.8294L68.584,67.0052L63.3431,72.474L57.5488,67.0052C55.791,68.0686 54.2719,68.8173 52.9915,69.2513C50.8431,69.9674 48.2715,70.3255 45.2767,70.3255C39.0267,70.3255 33.8618,68.4592 29.7819,64.7266C24.834,60.2344 22.36,53.6372 22.36,44.9349C22.36,36.1675 24.8991,29.5378 29.9772,25.0456C34.1222,21.378 39.2763,19.5443 45.4395,19.5443C51.6461,19.5443 56.8544,21.4865 61.0645,25.3711C65.9256,29.8633 68.3561,36.1458 68.3561,44.2188C68.3561,48.4939 67.8353,52.0747 66.7936,54.9609Z" style="fill:black;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'R',
		svg: `<g id="R">
        <circle cx="45" cy="45" r="45" style="fill:rgb(252,204,10);"/>
        <path d="M36.0319,29.3424L36.0319,42.2331L47.3926,42.2331C49.6495,42.2331 51.3422,41.9727 52.4707,41.4518C54.4672,40.5404 55.4655,38.7391 55.4655,36.0482C55.4655,33.1402 54.4998,31.1871 52.5684,30.1888C51.4833,29.6246 49.8557,29.3424 47.6855,29.3424L36.0319,29.3424ZM57.5326,22.2461C59.3446,23.0056 60.88,24.1233 62.1387,25.599C63.1803,26.8142 64.005,28.1597 64.6126,29.6354C65.2203,31.1111 65.5241,32.793 65.5241,34.681C65.5241,36.9596 64.949,39.2003 63.7988,41.403C62.6487,43.6057 60.7498,45.1628 58.1022,46.0742C60.3158,46.964 61.8837,48.2281 62.806,49.8665C63.7283,51.505 64.1895,54.0061 64.1895,57.3698L64.1895,60.5924C64.1895,62.7843 64.2763,64.2708 64.4499,65.0521C64.7103,66.2891 65.3179,67.2005 66.2728,67.7865L66.2728,68.9909L55.2376,68.9909C54.9338,67.9275 54.7168,67.0703 54.5866,66.4193C54.3262,65.0738 54.1851,63.6957 54.1634,62.2852L54.0983,57.8255C54.0549,54.7656 53.4961,52.7257 52.4219,51.7057C51.3477,50.6858 49.3349,50.1758 46.3835,50.1758L36.0319,50.1758L36.0319,68.9909L26.2337,68.9909L26.2337,21.0091L49.7689,21.0091C53.1326,21.0742 55.7205,21.4865 57.5326,22.2461Z" style="fill:black;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'GS',
		svg: `<g id="S">
        <circle cx="45" cy="45" r="45" style="fill:rgb(128,129,131);"/>
        <path d="M35.1042,54.1797C35.408,56.3715 36.0048,58.01 36.8945,59.0951C38.5221,61.0699 41.3108,62.0573 45.2604,62.0573C47.6259,62.0573 49.5464,61.7969 51.0221,61.276C53.8216,60.2778 55.2214,58.4223 55.2214,55.7096C55.2214,54.1254 54.5269,52.8993 53.138,52.0313C51.7491,51.1849 49.5681,50.4362 46.5951,49.7852L41.5169,48.6458C36.5256,47.5174 33.0751,46.2912 31.1654,44.9674C27.9319,42.7539 26.3151,39.2925 26.3151,34.5833C26.3151,30.2865 27.8776,26.7166 31.0026,23.8737C34.1276,21.0308 38.7174,19.6094 44.7721,19.6094C49.8286,19.6094 54.1417,20.9494 57.7116,23.6296C61.2815,26.3097 63.1532,30.1997 63.3268,35.2995L53.6914,35.2995C53.5178,32.4132 52.2591,30.3624 49.9154,29.1471C48.3529,28.3442 46.4106,27.9427 44.0885,27.9427C41.5061,27.9427 39.4444,28.4635 37.9036,29.5052C36.3628,30.5469 35.5924,32.0009 35.5924,33.8672C35.5924,35.5816 36.352,36.862 37.8711,37.7083C38.8477,38.2726 40.931,38.9345 44.1211,39.694L52.3893,41.6797C56.0135,42.5477 58.7478,43.7088 60.5924,45.1628C63.457,47.4197 64.8893,50.6858 64.8893,54.9609C64.8893,59.3446 63.2129,62.985 59.86,65.8822C56.5072,68.7793 51.7708,70.2279 45.651,70.2279C39.401,70.2279 34.4857,68.801 30.9049,65.9473C27.3242,63.0935 25.5339,59.171 25.5339,54.1797L35.1042,54.1797Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: 'FS',
		svg: `<g id="SF">
        <circle cx="45" cy="45" r="45" style="fill:#808183;" />
        <path
            d="M54.545,19.647l19.559,0l0,4.848l-13.832,0l0,6.345l12.11,0l0,4.792l-12.11,0l0,11.567l-5.727,0l0,-27.552Z"
            style="fill:#fff;fill-rule:nonzero;" />
        <path
            d="M22.003,54.18c0.304,2.192 0.901,3.83 1.791,4.915c1.627,1.975 4.416,2.962 8.366,2.962c2.365,0 4.286,-0.26 5.761,-0.781c2.8,-0.998 4.2,-2.854 4.2,-5.566c0,-1.585 -0.695,-2.811 -2.084,-3.679c-1.389,-0.846 -3.57,-1.595 -6.543,-2.246l-5.078,-1.139c-4.991,-1.129 -8.442,-2.355 -10.351,-3.679c-3.234,-2.213 -4.851,-5.674 -4.851,-10.384c0,-4.297 1.563,-7.866 4.688,-10.709c3.125,-2.843 7.715,-4.265 13.769,-4.265c5.057,0 9.37,1.34 12.94,4.021c3.57,2.68 5.441,6.57 5.615,11.669l-9.635,0c-0.174,-2.886 -1.433,-4.937 -3.776,-6.152c-1.563,-0.803 -3.505,-1.204 -5.827,-1.204c-2.583,0 -4.644,0.521 -6.185,1.562c-1.541,1.042 -2.311,2.496 -2.311,4.362c0,1.715 0.759,2.995 2.278,3.841c0.977,0.565 3.06,1.226 6.25,1.986l8.268,1.986c3.625,0.868 6.359,2.029 8.204,3.483c2.864,2.257 4.296,5.523 4.296,9.798c0,4.384 -1.676,8.024 -5.029,10.921c-3.353,2.897 -8.089,4.346 -14.209,4.346c-6.25,0 -11.165,-1.427 -14.746,-4.281c-3.581,-2.853 -5.371,-6.776 -5.371,-11.767l9.57,0Z"
            style="fill:#fff;fill-rule:nonzero;" />
    </g>`
	},
	{
		name: 'SI',
		svg: `<g id="SIR">
        <circle cx="45" cy="45" r="45" style="fill:#0078c6;" />
        <g>
            <path
                d="M15.369,52.053c0.217,1.559 0.641,2.725 1.274,3.497c1.158,1.405 3.142,2.107 5.952,2.107c1.683,0 3.049,-0.185 4.099,-0.556c1.992,-0.71 2.988,-2.03 2.988,-3.96c0,-1.127 -0.494,-1.999 -1.482,-2.617c-0.989,-0.602 -2.54,-1.135 -4.655,-1.598l-3.613,-0.81c-3.551,-0.803 -6.006,-1.676 -7.365,-2.617c-2.3,-1.575 -3.45,-4.038 -3.45,-7.388c0,-3.057 1.111,-5.597 3.334,-7.619c2.224,-2.023 5.489,-3.034 9.797,-3.034c3.597,0 6.666,0.953 9.205,2.86c2.54,1.907 3.872,4.674 3.995,8.302l-6.855,0c-0.123,-2.053 -1.019,-3.512 -2.686,-4.377c-1.112,-0.571 -2.494,-0.857 -4.146,-0.857c-1.837,0 -3.304,0.371 -4.4,1.112c-1.096,0.741 -1.644,1.776 -1.644,3.103c0,1.22 0.54,2.131 1.621,2.733c0.695,0.402 2.177,0.872 4.447,1.413l5.882,1.413c2.578,0.617 4.524,1.443 5.836,2.478c2.038,1.605 3.057,3.929 3.057,6.97c0,3.119 -1.193,5.709 -3.578,7.77c-2.385,2.061 -5.755,3.092 -10.109,3.092c-4.447,0 -7.944,-1.015 -10.491,-3.045c-2.548,-2.031 -3.821,-4.821 -3.821,-8.372l6.808,0Z"
                style="fill:#fff;fill-rule:nonzero;" />
            <rect x="40.159" y="28.454" width="7.087" height="34.137" style="fill:#fff;fill-rule:nonzero;" />
            <path
                d="M59.9,34.382l0,9.171l8.083,0c1.605,0 2.81,-0.185 3.612,-0.556c1.421,-0.648 2.131,-1.93 2.131,-3.844c0,-2.069 -0.687,-3.458 -2.061,-4.169c-0.772,-0.401 -1.93,-0.602 -3.474,-0.602l-8.291,0Zm15.297,-5.048c1.289,0.54 2.381,1.335 3.277,2.385c0.741,0.865 1.327,1.822 1.76,2.872c0.432,1.05 0.648,2.246 0.648,3.589c0,1.621 -0.409,3.216 -1.227,4.783c-0.819,1.567 -2.169,2.675 -4.053,3.323c1.575,0.633 2.69,1.532 3.346,2.698c0.657,1.166 0.985,2.945 0.985,5.338l0,2.293c0,1.559 0.061,2.617 0.185,3.173c0.185,0.88 0.618,1.528 1.297,1.945l0,0.857l-7.851,0c-0.216,-0.757 -0.371,-1.366 -0.463,-1.83c-0.186,-0.957 -0.286,-1.937 -0.301,-2.941l-0.047,-3.173c-0.031,-2.177 -0.428,-3.628 -1.192,-4.353c-0.765,-0.726 -2.197,-1.089 -4.296,-1.089l-7.365,0l0,13.386l-6.971,0l0,-34.136l16.744,0c2.393,0.046 4.234,0.339 5.524,0.88Z"
                style="fill:#fff;fill-rule:nonzero;" />
        </g>
    </g>`
	},
	{
		name: 'H',
		svg: `<g id="SR">
        <circle cx="45" cy="45" r="45" style="fill:#808183;" />
        <path
            d="M60.178,24.401l0,7.412l6.533,0c1.298,0 2.271,-0.15 2.92,-0.449c1.148,-0.524 1.722,-1.56 1.722,-3.107c0,-1.672 -0.555,-2.795 -1.666,-3.369c-0.624,-0.325 -1.56,-0.487 -2.808,-0.487l-6.701,0Zm7.899,-4.792c1.934,0.038 3.422,0.275 4.464,0.712c1.042,0.436 1.925,1.079 2.649,1.928c0.599,0.698 1.073,1.472 1.422,2.32c0.35,0.849 0.524,1.816 0.524,2.902c0,1.31 -0.33,2.598 -0.992,3.865c-0.661,1.266 -1.753,2.162 -3.275,2.686c1.273,0.511 2.174,1.238 2.705,2.18c0.53,0.943 0.795,2.381 0.795,4.315l0,1.853c0,1.26 0.05,2.115 0.15,2.564c0.15,0.711 0.499,1.235 1.048,1.572l0,0.693l-6.345,0c-0.175,-0.612 -0.3,-1.104 -0.375,-1.479c-0.149,-0.773 -0.23,-1.566 -0.243,-2.377l-0.037,-2.564c-0.025,-1.76 -0.347,-2.933 -0.964,-3.519c-0.618,-0.587 -1.775,-0.88 -3.472,-0.88l-5.953,0l0,10.819l-5.633,0l0,-27.59l13.532,0Z"
            style="fill:#fff;fill-rule:nonzero;" />
        <path
            d="M22.003,54.18c0.304,2.192 0.901,3.83 1.791,4.915c1.627,1.975 4.416,2.962 8.366,2.962c2.365,0 4.286,-0.26 5.761,-0.781c2.8,-0.998 4.2,-2.854 4.2,-5.566c0,-1.585 -0.695,-2.811 -2.084,-3.679c-1.389,-0.846 -3.57,-1.595 -6.543,-2.246l-5.078,-1.139c-4.991,-1.129 -8.442,-2.355 -10.351,-3.679c-3.234,-2.213 -4.851,-5.674 -4.851,-10.384c0,-4.297 1.563,-7.866 4.688,-10.709c3.125,-2.843 7.715,-4.265 13.769,-4.265c5.057,0 9.37,1.34 12.94,4.021c3.57,2.68 5.441,6.57 5.615,11.669l-9.635,0c-0.174,-2.886 -1.433,-4.937 -3.776,-6.152c-1.563,-0.803 -3.505,-1.204 -5.827,-1.204c-2.583,0 -4.644,0.521 -6.185,1.562c-1.541,1.042 -2.311,2.496 -2.311,4.362c0,1.715 0.759,2.995 2.278,3.841c0.977,0.565 3.06,1.226 6.25,1.986l8.268,1.986c3.625,0.868 6.359,2.029 8.204,3.483c2.864,2.257 4.296,5.523 4.296,9.798c0,4.384 -1.676,8.024 -5.029,10.921c-3.353,2.897 -8.089,4.346 -14.209,4.346c-6.25,0 -11.165,-1.427 -14.746,-4.281c-3.581,-2.853 -5.371,-6.776 -5.371,-11.767l9.57,0Z"
            style="fill:#fff;fill-rule:nonzero;" />
    </g>`
	},
	{
		name: 'W',
		svg: `<g transform="matrix(1,0,0,1,-5,-5)">
        <g id="W">
            <g transform="matrix(1.02285,0,0,1.02285,-0.474267,-1.17657)">
                <circle cx="49.347" cy="50.033" r="43.995" style="fill:rgb(252,204,10);"/>
            </g>
            <g transform="matrix(1.26802,0,0,1.26802,21.9029,39.5201)">
                <path d="M6.511,-10.655L11.517,11.012L12.596,17.044L13.699,11.14L17.961,-10.655L26.304,-10.655L30.797,11.012L31.952,17.044L33.107,11.243L38.164,-10.655L46.2,-10.655L35.546,27.185L27.998,27.185L23.429,5.056L22.094,-2.261L20.759,5.056L16.19,27.185L8.847,27.185L-1.883,-10.655L6.511,-10.655Z" style="fill:black;fill-rule:nonzero;"/>
            </g>
        </g>
    </g>`
	},
	{
		name: 'Z',
		svg: `<g id="Z">
        <circle cx="45" cy="45" r="45" style="fill:rgb(153,100,51);"/>
        <path d="M26.3314,60.5273L51.1035,29.5052L26.9499,29.5052L26.9499,21.0091L63.6035,21.0091L63.6035,29.0495L38.5059,60.5273L63.6686,60.5273L63.6686,68.9909L26.3314,68.9909L26.3314,60.5273Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	// these next icons are for parsing alert html
	{
		name: 'S',
		svg: `<g id="S">
        <circle cx="45" cy="45" r="45" style="fill:rgb(128,129,131);"/>
        <path d="M35.1042,54.1797C35.408,56.3715 36.0048,58.01 36.8945,59.0951C38.5221,61.0699 41.3108,62.0573 45.2604,62.0573C47.6259,62.0573 49.5464,61.7969 51.0221,61.276C53.8216,60.2778 55.2214,58.4223 55.2214,55.7096C55.2214,54.1254 54.5269,52.8993 53.138,52.0313C51.7491,51.1849 49.5681,50.4362 46.5951,49.7852L41.5169,48.6458C36.5256,47.5174 33.0751,46.2912 31.1654,44.9674C27.9319,42.7539 26.3151,39.2925 26.3151,34.5833C26.3151,30.2865 27.8776,26.7166 31.0026,23.8737C34.1276,21.0308 38.7174,19.6094 44.7721,19.6094C49.8286,19.6094 54.1417,20.9494 57.7116,23.6296C61.2815,26.3097 63.1532,30.1997 63.3268,35.2995L53.6914,35.2995C53.5178,32.4132 52.2591,30.3624 49.9154,29.1471C48.3529,28.3442 46.4106,27.9427 44.0885,27.9427C41.5061,27.9427 39.4444,28.4635 37.9036,29.5052C36.3628,30.5469 35.5924,32.0009 35.5924,33.8672C35.5924,35.5816 36.352,36.862 37.8711,37.7083C38.8477,38.2726 40.931,38.9345 44.1211,39.694L52.3893,41.6797C56.0135,42.5477 58.7478,43.7088 60.5924,45.1628C63.457,47.4197 64.8893,50.6858 64.8893,54.9609C64.8893,59.3446 63.2129,62.985 59.86,65.8822C56.5072,68.7793 51.7708,70.2279 45.651,70.2279C39.401,70.2279 34.4857,68.801 30.9049,65.9473C27.3242,63.0935 25.5339,59.171 25.5339,54.1797L35.1042,54.1797Z" style="fill:white;fill-rule:nonzero;"/>
    </g>`
	},
	{
		name: '7X',
		svg: `<g id="_7-Diamond" serif:id="7 Diamond">
        <g transform="matrix(0.539996,0.539996,-0.555556,0.555556,51.4781,-4.07742)">
            <rect x="2.407" y="5" width="92.593" height="90" style="fill:rgb(185,51,174);"/>
        </g>
        <g transform="matrix(1.26802,0,0,1.26802,21.9029,39.5201)">
            <path d="M35.264,-3.827C34.168,-2.748 32.645,-0.827 30.694,1.937C28.743,4.701 27.109,7.554 25.791,10.498C24.747,12.809 23.805,15.632 22.967,18.97C22.128,22.307 21.709,25.045 21.709,27.185L14.11,27.185C14.333,20.51 16.523,13.57 20.682,6.365C23.369,1.898 25.62,-1.217 27.434,-2.979L8.873,-2.979L8.976,-9.551L35.264,-9.551L35.264,-3.827Z" style="fill:white;fill-rule:nonzero;"/>
        </g>
    </g>`
	},
	// TODO: fix clipping for diamond icons
	{
		name: 'FX',
		svg: `<g id="F-Diamond" serif:id="F Diamond">
        <g transform="matrix(0.539996,0.539996,-0.555556,0.555556,51.4781,-4.07742)">
            <rect x="2.407" y="5" width="92.593" height="90" style="fill:rgb(255,98,25);"/>
        </g>
        <g transform="matrix(1.26802,0,0,1.26802,21.9029,39.5201)">
            <path d="M10.105,-10.604L36.932,-10.604L36.932,-3.955L17.961,-3.955L17.961,4.748L34.57,4.748L34.57,11.32L17.961,11.32L17.961,27.185L10.105,27.185L10.105,-10.604Z" style="fill:white;fill-rule:nonzero;"/>
        </g>
    </g>`
	},
	{
		name: '6X',
		svg: `<g id="_6-Diamond" serif:id="6 Diamond">
        <g transform="matrix(0.539996,0.539996,-0.555556,0.555556,51.4781,-4.07742)">
            <rect x="2.407" y="5" width="92.593" height="90" style="fill:rgb(0,147,59);"/>
        </g>
        <g transform="matrix(1.26802,0,0,1.26802,21.9029,39.5201)">
            <path d="M18.141,20.305C19.219,21.571 20.588,22.204 22.248,22.204C23.874,22.204 25.153,21.593 26.086,20.369C27.019,19.145 27.485,17.558 27.485,15.607C27.485,13.433 26.954,11.769 25.893,10.614C24.832,9.458 23.532,8.881 21.991,8.881C20.742,8.881 19.638,9.257 18.68,10.01C17.242,11.123 16.523,12.92 16.523,15.401C16.523,17.404 17.062,19.038 18.141,20.305ZM26.818,-0.669C26.818,-1.268 26.587,-1.927 26.124,-2.646C25.337,-3.809 24.148,-4.391 22.556,-4.391C20.177,-4.391 18.483,-3.056 17.473,-0.387C16.925,1.085 16.549,3.259 16.344,6.134C17.251,5.056 18.303,4.269 19.501,3.772C20.699,3.276 22.068,3.028 23.609,3.028C26.912,3.028 29.62,4.149 31.734,6.391C33.847,8.633 34.904,11.499 34.904,14.991C34.904,18.465 33.869,21.528 31.798,24.181C29.727,26.834 26.51,28.16 22.145,28.16C17.456,28.16 13.999,26.201 11.774,22.281C10.045,19.218 9.181,15.265 9.181,10.421C9.181,7.58 9.301,5.27 9.541,3.49C9.968,0.324 10.799,-2.312 12.031,-4.417C13.092,-6.214 14.482,-7.66 16.202,-8.756C17.922,-9.851 19.98,-10.398 22.376,-10.398C25.834,-10.398 28.589,-9.513 30.643,-7.741C32.696,-5.97 33.852,-3.613 34.108,-0.669L26.818,-0.669Z" style="fill:white;fill-rule:nonzero;"/>
        </g>
    </g>`
	},
	{
		// from lucide icons
		name: 'accessibility icon',
		complete_svg: true,
		svg: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor"
			stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="inline-block">
			<circle cx="16" cy="4" r="1" />
			<path d="m18 19 1-7-6 1" />
			<path d="m5 8 3-3 5.5 3-2.36 3.5" />
			<path d="M4.24 14.5a5 5 0 0 0 6.88 6" />
			<path d="M13.76 17.5a5 5 0 0 0-6.88-6" />
		</svg>`
	},
	{
		// from lucide icons
		name: 'shuttle bus icon',
		complete_svg: true,
		svg: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="inline-block"><path d="M4 6 2 7"/><path d="M10 6h4"/><path d="m22 7-2-1"/><rect width="16" height="16" x="4" y="3" rx="2"/><path d="M4 11h16"/><path d="M8 15h.01"/><path d="M16 15h.01"/><path d="M6 19v2"/><path d="M18 21v-2"/></svg>`
	},
	{
		// from lucide icons
		name: 'not found',
		complete_svg: true,
		svg: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="inline-block"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>`
	}
];

export default icons;
